package io.digua.token;

import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class TokenServerClientClientTest {
    private MockWebServer gw;
    private TokenServerClientClient client;

    @BeforeEach
    void setUp() throws IOException {
        gw = new MockWebServer();
        gw.start();

        gw.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"access_token\":\"tc\",\"refresh_token\":\"tr\",\"token_type\":\"Bearer\",\"expires_in\":300}"));

        client = TokenServerClientClient.create(gw.url("/").toString(), "ak", "sec", "code-1");
    }

    @AfterEach
    void tearDown() throws IOException { gw.shutdown(); }

    @Test
    void serviceJwtGrantManagementFlow() throws Exception {
        // create
        gw.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"owner\":\"svc-a\",\"audience\":\"iam-service\",\"status\":\"active\",\"system_managed\":false}"));
        ServiceJwtGrantInfo created = client.createServiceJwtGrant("svc-a", "iam-service");
        assertEquals("svc-a", created.getOwner());
        assertEquals("active", created.getStatus());
        assertFalse(created.isSystemManaged());

        // list（带过滤）
        gw.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"items\":[{\"owner\":\"svc-a\",\"audience\":\"iam-service\",\"status\":\"active\",\"system_managed\":false}],\"total\":1,\"page\":1,\"size\":20}"));
        ServiceJwtGrantPage page = client.listServiceJwtGrants("svc-a", null, "active", 1, 20);
        assertEquals(1, page.getTotal());
        assertEquals("iam-service", page.getItems().get(0).getAudience());

        // get
        gw.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"owner\":\"svc-a\",\"audience\":\"iam-service\",\"status\":\"active\",\"system_managed\":true}"));
        ServiceJwtGrantInfo got = client.getServiceJwtGrant("svc-a", "iam-service");
        assertTrue(got.isSystemManaged());

        // disable
        gw.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"owner\":\"svc-a\",\"audience\":\"iam-service\",\"status\":\"disabled\",\"system_managed\":false}"));
        ServiceJwtGrantInfo disabled =
                client.setServiceJwtGrantStatus("svc-a", "iam-service", "disabled");
        assertEquals("disabled", disabled.getStatus());

        // delete（204）
        gw.enqueue(new MockResponse().setResponseCode(204));
        client.deleteServiceJwtGrant("svc-a", "iam-service");

        // 请求序列：access(client_credentials) → POST → GET(list) → GET(one) → PUT → DELETE
        RecordedRequest access = gw.takeRequest();
        assertTrue(access.getPath().endsWith("/v1/tokens/access"));
        RecordedRequest create = gw.takeRequest();
        assertEquals("POST", create.getMethod());
        assertTrue(create.getPath().endsWith("/v1/admin/service-jwt-grants"));
        assertTrue(create.getBody().readUtf8().contains("\"audience\":\"iam-service\""));
        RecordedRequest list = gw.takeRequest();
        assertTrue(list.getPath().endsWith("/v1/admin/service-jwt-grants?owner=svc-a&status=active&page=1&size=20"));
        RecordedRequest get = gw.takeRequest();
        assertTrue(get.getPath().endsWith("/v1/admin/service-jwt-grants/svc-a/iam-service"));
        RecordedRequest put = gw.takeRequest();
        assertEquals("PUT", put.getMethod());
        assertTrue(put.getPath().endsWith("/v1/admin/service-jwt-grants/svc-a/iam-service/status"));
        RecordedRequest del = gw.takeRequest();
        assertEquals("DELETE", del.getMethod());
        assertTrue(del.getPath().endsWith("/v1/admin/service-jwt-grants/svc-a/iam-service"));
    }

    @Test
    void createApiKeyAfterLogin() throws Exception {
        gw.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"api_key_id\":\"ak-2\",\"api_key_secret\":\"sec-2\"}"));

        ApiKeyInfo key = client.createApiKey("backend-x",
                java.util.Arrays.asList("notification.admin"),
                java.util.Arrays.asList("client_credentials"));
        assertEquals("ak-2", key.getApiKeyId());
        assertEquals("sec-2", key.getApiKeySecret());

        RecordedRequest login = gw.takeRequest();
        assertTrue(login.getPath().endsWith("/v1/tokens/access"));
        assertEquals("application/json", login.getHeader("Content-Type"));

        RecordedRequest createKey = gw.takeRequest();
        assertEquals("POST", createKey.getMethod());
        assertTrue(createKey.getPath().endsWith("/v1/api-keys"));
        assertEquals("application/json", createKey.getHeader("Content-Type"));
        String body = createKey.getBody().readUtf8();
        assertTrue(body.contains("\"name\":\"backend-x\""));
        assertTrue(body.contains("\"permissions\":[\"notification.admin\"]"));
        assertTrue(body.contains("\"grant_types\":[\"client_credentials\"]"));
    }

    // ── R1-03：URL 编码——断言最终发出的 HTTP 请求路径与 Query ────────────────

    // 组合攻击字符（/ ? # & % 结构字符 + Unicode）。
    private static final String HOSTILE = "a/b?c#d&e%f服务";
    // 期望的 percent-encoding（UTF-8 逐字节：服=E6 9C 8D、务=E5 8A A1）。
    private static final String HOSTILE_ENCODED = "a%2Fb%3Fc%23d%26e%25f%E6%9C%8D%E5%8A%A1";
    // takeRequest 超时：无参 takeRequest 会无限阻塞，请求错位时挂死整个 surefire。
    private static final long REQUEST_TIMEOUT_SECONDS = 5;

    private void enqueueGrant(String json) {
        gw.enqueue(new MockResponse().setResponseCode(200).setBody(json));
    }

    /**
     * 预热 token 缓存：消耗一次 client_credentials access 请求
     * （后续业务调用不再带 access，请求一一对应、断言不错位）。
     */
    private void warmTokenCache() throws Exception {
        gw.enqueue(new MockResponse().setResponseCode(200).setBody("{}"));
        client.jwks();
        gw.takeRequest(REQUEST_TIMEOUT_SECONDS, TimeUnit.SECONDS); // access(client_credentials)
        gw.takeRequest(REQUEST_TIMEOUT_SECONDS, TimeUnit.SECONDS); // jwks
    }

    /** 取下一个请求的 path（带超时——无参 takeRequest 会无限阻塞）。 */
    private String nextPath() throws InterruptedException {
        return gw.takeRequest(REQUEST_TIMEOUT_SECONDS, TimeUnit.SECONDS).getPath();
    }

    @Test
    void hostilePathParamsCannotChangeRequestStructure() throws Exception {
        warmTokenCache();
        String grantJson = "{\"owner\":\"x\",\"audience\":\"y\",\"status\":\"active\",\"system_managed\":false}";

        enqueueGrant(grantJson);
        client.getServiceJwtGrant(HOSTILE, HOSTILE);
        String expectedGet = "/v1/admin/service-jwt-grants/" + HOSTILE_ENCODED + "/" + HOSTILE_ENCODED;
        assertTrue(nextPath().endsWith(expectedGet),
                "GET path must keep owner/audience as single encoded segments");

        enqueueGrant(grantJson);
        client.setServiceJwtGrantStatus(HOSTILE, HOSTILE, "disabled");
        String expectedPut = "/v1/admin/service-jwt-grants/" + HOSTILE_ENCODED + "/" + HOSTILE_ENCODED + "/status";
        assertTrue(nextPath().endsWith(expectedPut),
                "PUT path must keep owner/audience as single encoded segments");

        gw.enqueue(new MockResponse().setResponseCode(204));
        client.deleteServiceJwtGrant(HOSTILE, HOSTILE);
        String expectedDelete = "/v1/admin/service-jwt-grants/" + HOSTILE_ENCODED + "/" + HOSTILE_ENCODED;
        assertTrue(nextPath().endsWith(expectedDelete),
                "DELETE path must keep owner/audience as single encoded segments");
    }

    @Test
    void hostileQueryParamsCannotChangeRequestStructure() throws Exception {
        warmTokenCache();
        enqueueGrant("{\"items\":[],\"total\":0,\"page\":2,\"size\":5}");
        client.listServiceJwtGrants(HOSTILE, HOSTILE, "active", 2, 5);
        String path = nextPath();
        String expectedQuery = "owner=" + HOSTILE_ENCODED
                + "&audience=" + HOSTILE_ENCODED + "&status=active&page=2&size=5";
        assertTrue(path.endsWith("/v1/admin/service-jwt-grants?" + expectedQuery),
                "query must be percent-encoded so & / = / % cannot change its structure: " + path);
    }

    @Test
    void plainParamsKeepSemanticsAfterEncoding() throws Exception {
        warmTokenCache();
        enqueueGrant("{\"owner\":\"svc-a\",\"audience\":\"aud_1\",\"status\":\"active\",\"system_managed\":false}");
        client.getServiceJwtGrant("svc-a", "aud_1");
        assertTrue(nextPath().endsWith("/v1/admin/service-jwt-grants/svc-a/aud_1"),
                "unreserved characters must stay verbatim");

        enqueueGrant("{\"items\":[],\"total\":0,\"page\":1,\"size\":20}");
        client.listServiceJwtGrants("svc-a", "aud_1", "active", 1, 20);
        assertTrue(nextPath().endsWith("/v1/admin/service-jwt-grants?owner=svc-a&audience=aud_1&status=active&page=1&size=20"),
                "normal query must stay verbatim");
    }
}
