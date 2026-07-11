package com.kudi.coder.pmd.repo;

import com.kudi.coder.pmd.domain.Milestone;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;

import java.util.Optional;

public interface MilestoneRepository extends JpaRepository<Milestone, Long> {

    Optional<Milestone> findByIdAndTenantId(Long id, Long tenantId);

    Page<Milestone> findAllByTenantIdAndProjectId(Long tenantId, Long projectId, Pageable pageable);
}
