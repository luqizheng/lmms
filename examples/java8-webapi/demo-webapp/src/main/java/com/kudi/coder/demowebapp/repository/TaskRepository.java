package com.kudi.coder.demowebapp.repository;

import com.kudi.coder.demowebapp.domain.Task;
import org.springframework.data.jpa.repository.JpaRepository;

public interface TaskRepository extends JpaRepository<Task, Long> {
}