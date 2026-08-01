package com.kudi.coder.sdk.user;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;

import java.util.List;

@JsonIgnoreProperties(ignoreUnknown = true)
public class BatchGetUsersRequest {
    private List<String> userIds;

    public BatchGetUsersRequest() {}

    public BatchGetUsersRequest(List<String> userIds) {
        this.userIds = userIds;
    }

    public List<String> getUserIds() { return userIds; }
    public void setUserIds(List<String> userIds) { this.userIds = userIds; }
}
