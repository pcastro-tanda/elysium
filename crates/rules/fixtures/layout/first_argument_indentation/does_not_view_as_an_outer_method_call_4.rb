@subject_results[subject] = original.update(
                              mutation_results: (dup << mutation_result),
                              tests:            test_result.tests
)
