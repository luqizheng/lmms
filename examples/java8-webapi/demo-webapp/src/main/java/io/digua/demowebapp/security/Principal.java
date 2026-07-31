package io.digua.security;

import java.util.Collections;
import java.util.List;

public class Principal {
    private final String userId;
    private final List<String> permissions;

    public Principal(String userId, List<String> permissions) {
        this.userId = userId;
        this.permissions = permissions == null ? Collections.emptyList() : permissions;
    }

    public String getUserId() { return userId; }
    public List<String> getPermissions() { return permissions; }

    public boolean hasPermission(String code) {
        return permissions.contains(code);
    }
}