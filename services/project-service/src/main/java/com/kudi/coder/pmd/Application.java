package com.kudi.coder.pmd;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.boot.autoconfigure.domain.EntityScan;
import org.springframework.data.jpa.repository.config.EnableJpaRepositories;

@SpringBootApplication(scanBasePackages = {
        "com.kudi.coder.pmd",
        "com.kudi.coder.common"
})
@EntityScan(basePackages = {
        "com.kudi.coder.pmd.domain",
        "com.kudi.coder.pmd.outbox",
        "com.kudi.coder.pmd.llm"
})
@EnableJpaRepositories(basePackages = "com.kudi.coder.pmd.repo")
public class Application {
    public static void main(String[] args) {
        SpringApplication.run(Application.class, args);
    }
}
