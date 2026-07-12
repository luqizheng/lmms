package com.coder.pm.infra.persistence.mapper;

import com.baomidou.mybatisplus.core.mapper.BaseMapper;
import com.coder.pm.infra.persistence.entity.KbDocumentVersion;
import org.apache.ibatis.annotations.Mapper;

@Mapper
public interface KbDocumentVersionMapper extends BaseMapper<KbDocumentVersion> {
}
