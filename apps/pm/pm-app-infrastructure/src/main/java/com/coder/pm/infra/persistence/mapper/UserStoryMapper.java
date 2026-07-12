package com.coder.pm.infra.persistence.mapper;

import com.baomidou.mybatisplus.core.mapper.BaseMapper;
import com.coder.pm.infra.persistence.entity.UserStory;
import org.apache.ibatis.annotations.Mapper;

@Mapper
public interface UserStoryMapper extends BaseMapper<UserStory> {
}
