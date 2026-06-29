package com.kudi.coder.demowebapp.config;

import org.springframework.boot.context.properties.ConfigurationProperties;

@ConfigurationProperties(prefix = "kudi")
public class KudiCoderProperties {

    private Gateway gateway = new Gateway();
    private Iam iam = new Iam();
    private Client client = new Client();

    public Gateway getGateway() { return gateway; }
    public void setGateway(Gateway gateway) { this.gateway = gateway; }
    public Iam getIam() { return iam; }
    public void setIam(Iam iam) { this.iam = iam; }
    public Client getClient() { return client; }
    public void setClient(Client client) { this.client = client; }

    public static class Gateway {
        private String baseUrl = "http://localhost:3000";
        public String getBaseUrl() { return baseUrl; }
        public void setBaseUrl(String baseUrl) { this.baseUrl = baseUrl; }
    }

    public static class Iam {
        private String username;
        private String password;
        public String getUsername() { return username; }
        public void setUsername(String username) { this.username = username; }
        public String getPassword() { return password; }
        public void setPassword(String password) { this.password = password; }
    }

    public static class Client {
        private String apiKey;
        private String apiSecret;
        public String getApiKey() { return apiKey; }
        public void setApiKey(String apiKey) { this.apiKey = apiKey; }
        public String getApiSecret() { return apiSecret; }
        public void setApiSecret(String apiSecret) { this.apiSecret = apiSecret; }
    }
}