def test
  if something && something_that_makes_the_guard_clause_too_long_to_fit_on_one_line
  ^^ Use a guard clause (`unless something && something_that_makes_the_guard_clause_too_long_to_fit_on_one_line; return; end`) instead of wrapping the code inside a conditional expression.
    work
    more_work
  end
end
