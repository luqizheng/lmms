package com.coder.pm.web.security;

import com.coder.pm.domain.security.Principal;
import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpServer;

import java.io.IOException;
import java.io.OutputStream;
import java.math.BigInteger;
import java.net.InetSocketAddress;
import java.security.interfaces.RSAPublicKey;
import java.util.Base64;

/**
 * Tiny in-process HTTP server that serves a single JWKS document for unit
 * tests. Used by {@link JwksJwtVerifierTest}. Not part of production code.
 */
final class FakeJwksServer {
    private final HttpServer server;
    private final int port;

    private FakeJwksServer(HttpServer server, int port) {
        this.server = server;
        this.port = port;
    }

    static FakeJwksServer start(RSAPublicKey key, String kid) throws IOException {
        HttpServer srv = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
        String jwks = buildJwksJson(key, kid);
        srv.createContext("/.well-known/jwks.json", exchange -> {
            byte[] bytes = jwks.getBytes();
            exchange.getResponseHeaders().add("Content-Type", "application/json");
            exchange.sendResponseHeaders(200, bytes.length);
            try (OutputStream out = exchange.getResponseBody()) { out.write(bytes); }
        });
        srv.start();
        return new FakeJwksServer(srv, srv.getAddress().getPort());
    }

    int port() { return port; }

    void stop() { server.stop(0); }

    private static String buildJwksJson(RSAPublicKey key, String kid) {
        BigInteger n = key.getModulus();
        BigInteger e = key.getPublicExponent();
        String nB64 = Base64.getUrlEncoder().withoutPadding().encodeToString(toUnsigned(n.toByteArray()));
        String eB64 = Base64.getUrlEncoder().withoutPadding().encodeToString(toUnsigned(e.toByteArray()));
        return String.format(
                "{\"keys\":[{\"kty\":\"RSA\",\"alg\":\"RS256\",\"use\":\"sig\",\"kid\":\"%s\",\"n\":\"%s\",\"e\":\"%s\"}]}",
                kid, nB64, eB64);
    }

    private static byte[] toUnsigned(byte[] in) {
        if (in.length > 1 && in[0] == 0) {
            byte[] out = new byte[in.length - 1];
            System.arraycopy(in, 1, out, 0, out.length);
            return out;
        }
        return in;
    }
}