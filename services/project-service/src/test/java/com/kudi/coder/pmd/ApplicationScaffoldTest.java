package com.kudi.coder.pmd;

import org.junit.jupiter.api.Test;

/**
 * Phase 9 T2 scaffold smoke. Real handler/service tests land with T6/T7.
 */
class ApplicationScaffoldTest {

    @Test
    void mainClassIsPresent() {
        // Compile-time existence check: Application.main(String[]) is reachable
        // from this test classpath because both live under com.kudi.coder.pmd.
        // The smoke itself is trivial; the value is failing the build when
        // someone removes Application.java by accident.
        Class<?> main = Application.class;
        org.junit.jupiter.api.Assertions.assertNotNull(main);
    }
}
