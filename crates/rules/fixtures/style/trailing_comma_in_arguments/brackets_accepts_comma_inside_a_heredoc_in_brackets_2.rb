expect_no_offenses(
  expect_no_offenses(<<~SOURCE)
    run[
          :foo, defaults.merge(
                                bar: 3)]
  SOURCE
)
