package com.kudi.coder.pmd.repo;

import com.kudi.coder.pmd.domain.Project;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;

import java.util.Optional;

public interface ProjectRepository extends JpaRepository<Project, Long> {

    Optional<Project> findByIdAndTenantId(Long id, Long tenantId);

    Page<Project> findAllByTenantId(Long tenantId, Pageable pageable);

    boolean existsByCodeAndTenantId(String code, Long tenantId);
}
