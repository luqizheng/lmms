package com.kudi.coder.pmd.repo;

import com.kudi.coder.pmd.domain.Task;
import com.kudi.coder.pmd.domain.TaskStatus;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Modifying;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

import java.util.Optional;

public interface TaskRepository extends JpaRepository<Task, Long> {

    Optional<Task> findByIdAndTenantId(Long id, Long tenantId);

    Page<Task> findAllByTenantId(Long tenantId, Pageable pageable);

    Page<Task> findAllByTenantIdAndStatus(Long tenantId, TaskStatus status, Pageable pageable);

    Page<Task> findAllByTenantIdAndProjectId(Long tenantId, Long projectId, Pageable pageable);

    @Modifying
    @Query("update Task t set t.status = :newStatus where t.id = :id and t.tenantId = :tenantId")
    int transitionStatus(@Param("id") Long id,
                         @Param("tenantId") Long tenantId,
                         @Param("newStatus") TaskStatus newStatus);
}
