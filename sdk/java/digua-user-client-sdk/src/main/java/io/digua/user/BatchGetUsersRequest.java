package io.digua.user;

import com.fasterxml.jackson.annotation.JsonProperty;

import java.util.List;

public class BatchGetUsersRequest {
    @JsonProperty("user_ids")
    private List<String> userIds;

    public BatchGetUsersRequest() {}

    public BatchGetUsersRequest(List<String> userIds) {
        this.userIds = userIds;
    }

    public List<String> getUserIds() { return userIds; }
    public void setUserIds(List<String> userIds) { this.userIds = userIds; }
}
