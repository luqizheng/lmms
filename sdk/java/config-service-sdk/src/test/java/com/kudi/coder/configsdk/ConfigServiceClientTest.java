package com.kudi.coder.configsdk;

import com.kudi.coder.configsdk.exception.ConfigAuthException;
import com.kudi.coder.configsdk.exception.ConfigException;
import com.kudi.coder.configsdk.model.ConfigItem;
import com.kudi.coder.configsdk.model.ConfigItemQuery;
import com.kudi.coder.configsdk.model.ConfigItemRequest;
import com.kudi.coder.configsdk.model.ConfigItemUpdate;
import com.kudi.coder.configsdk.model.Namespace;
import com.kudi.coder.configsdk.model.NamespaceQuery;
import com.kudi.coder.configsdk.model.NamespaceRequest;
import com.kudi.coder.configsdk.model.PageResult;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ConfigServiceClientTest {

    private MockWebServer server;
    private ConfigServiceClient client;
    private AtomicReference<String> bearer;

    @BeforeEach
    void setUp() throws Exception {
        server = new MockWebServer();
        server.start();
        bearer = new AtomicReference<>("test-token");
        client = new ConfigServiceClient(server.url("/").toString(), bearer::get);
    }

    @AfterEach
    void tearDown() throws Exception {
        server.shutdown();
    }

    @Test
    void createNamespace_returns_namespace_on_201() throws Exception {
        server.enqueue(new MockResponse()
                .setResponseCode(201)
                .setBody("{\"id\":1,\"app\":\"pm\",\"name\":\"llm\"}")
                .addHeader("Content-Type", "application/json"));

        Namespace created = client.createNamespace(
                NamespaceRequest.builder().app("pm").name("llm").description("LLM config").build());
        assertEquals(1L, created.getId());
        assertEquals("llm", created.getName());

        RecordedRequest req = server.takeRequest();
        assertEquals("POST", req.getMethod());
        assertEquals("/v1/config/namespaces", req.getPath());
        assertEquals("Bearer test-token", req.getHeader("Authorization"));
        assertTrue(req.getBody().readUtf8().contains("\"app\":\"pm\""));
    }

    @Test
    void queryConfigItems_parses_pagination_with_namespace_id() throws Exception {
        server.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"items\":[{\"id\":1,\"namespaceId\":42,\"key\":\"provider.default\"}],\"total\":1,\"page\":1,\"size\":20}")
                .addHeader("Content-Type", "application/json"));

        ConfigItemQuery q = new ConfigItemQuery();
        q.setKey("provider.default");
        PageResult<ConfigItem> page = client.queryConfigItems(42L, q);
        assertEquals(1L, page.getTotal());
        assertEquals("provider.default", page.getItems().get(0).getKey());

        RecordedRequest req = server.takeRequest();
        assertTrue(req.getPath().startsWith("/v1/config/namespaces/42/items?"));
        assertTrue(req.getPath().contains("key=provider.default"));
    }

    @Test
    void updateConfigItem_sends_PUT_with_body() throws Exception {
        server.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"id\":7,\"key\":\"k\",\"value\":\"v2\"}")
                .addHeader("Content-Type", "application/json"));

        ConfigItem updated = client.updateConfigItem(7L,
                ConfigItemUpdate.builder().value("v2").valueType("string").build());
        assertEquals("v2", updated.getValue());

        RecordedRequest req = server.takeRequest();
        assertEquals("PUT", req.getMethod());
        assertEquals("/v1/config/items/7", req.getPath());
        assertTrue(req.getBody().readUtf8().contains("\"value\":\"v2\""));
    }

    @Test
    void deleteConfigItem_sends_DELETE() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(200).setBody(""));
        client.deleteConfigItem(11L);
        RecordedRequest req = server.takeRequest();
        assertEquals("DELETE", req.getMethod());
        assertEquals("/v1/config/items/11", req.getPath());
    }

    @Test
    void queryNamespaces_parses_app_filter() throws Exception {
        server.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"items\":[{\"id\":1,\"app\":\"pm\",\"name\":\"x\"}],\"total\":1,\"page\":1,\"size\":20}")
                .addHeader("Content-Type", "application/json"));

        NamespaceQuery q = new NamespaceQuery();
        q.setApp("pm");
        PageResult<Namespace> page = client.queryNamespaces(q);
        assertEquals(1L, page.getTotal());

        RecordedRequest req = server.takeRequest();
        assertTrue(req.getPath().startsWith("/v1/config/namespaces?"));
        assertTrue(req.getPath().contains("app=pm"));
    }

    @Test
    void createConfigItem_sends_POST_with_namespace_id_in_path() throws Exception {
        server.enqueue(new MockResponse()
                .setResponseCode(201)
                .setBody("{\"id\":1,\"namespaceId\":5,\"key\":\"k\"}")
                .addHeader("Content-Type", "application/json"));

        ConfigItem item = client.createConfigItem(5L,
                ConfigItemRequest.builder().key("k").value("v").build());
        assertEquals(5L, item.getNamespaceId());

        RecordedRequest req = server.takeRequest();
        assertEquals("POST", req.getMethod());
        assertEquals("/v1/config/namespaces/5/items", req.getPath());
    }

    @Test
    void getConfigItem_throws_AuthException_on_401() {
        server.enqueue(new MockResponse().setResponseCode(401).setBody("unauthorized"));
        assertThrows(ConfigAuthException.class, () -> client.getConfigItem(1L));
    }

    @Test
    void getConfigItem_throws_ConfigException_on_500() {
        server.enqueue(new MockResponse().setResponseCode(500).setBody("server err"));
        assertThrows(ConfigException.class, () -> client.getConfigItem(1L));
    }
}
