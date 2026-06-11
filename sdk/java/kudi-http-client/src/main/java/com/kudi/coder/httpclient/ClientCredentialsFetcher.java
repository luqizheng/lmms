package com.kudi.coder.httpclient;

import java.util.Map;
import java.util.function.Supplier;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.kudi.coder.httpclient.Exceptions.AuthException;
import com.kudi.coder.httpclient.Exceptions.NetworkException;
import com.kudi.coder.httpclient.Models.TokenInfo;

public class ClientCredentialsFetcher {
    private final HttpTransport transport;
    private final Supplier<String> tokenEndpointSupplier;
    private final ObjectMapper json = new ObjectMapper();

    public ClientCredentialsFetcher(HttpTransport transport, Supplier<String> tokenEndpointSupplier) {
        this.transport = transport;
        this.tokenEndpointSupplier = tokenEndpointSupplier;
    }

    public TokenInfo fetch(String clientId, String clientSecret) throws Exception {
        HttpTransport.Response resp = transport.postForm(tokenEndpointSupplier.get(), Map.of(
                "grant_type", "client_credentials",
                "client_id", clientId,
                "client_secret", clientSecret
        ));
        if (resp.statusCode < 200 || resp.statusCode >= 300) {
            throw new AuthException(
                    "client_credentials failed: HTTP " + resp.statusCode,
                    resp.statusCode,
                    resp.body);
        }
        TokenInfo info;
        try {
            info = json.readValue(resp.body, TokenInfo.class);
        } catch (Exception e) {
            throw new NetworkException("parse token response: " + e.getMessage(), e);
        }
        if (info.accessToken == null || info.accessToken.isEmpty()) {
            throw new AuthException("token response missing access_token", resp.statusCode, resp.body);
        }
        return info;
    }
}

