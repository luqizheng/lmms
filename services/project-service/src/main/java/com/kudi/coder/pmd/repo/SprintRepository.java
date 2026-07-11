package com.kudi.coder.pmd.repo;

import com.kudi.coder.pmd.domain.SprintMapping;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;

import java.util.Optional;

public interface SprintRepository extends JpaRepository<SprintMapping, Long> {

    Optional<SprintMapping> findByIdAndTenantId(Long id, Long tenantId);

    Page<SprintMapping> findAllByTenantId(Long tenantId, Pageable pageable);

    Page<SprintMapping> findAllByTenantIdAndProjectId(Long tenantId, Long projectId, Pageable pageable);
}
