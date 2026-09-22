def test
  if something && something_that_makes_the_guard_clause_too_long_to_fit_on_one_line
  ^^ Use a guard clause (`return unless something && something_that_makes_the_guard_clause_too_long_to_fit_on_one_line`) instead of wrapping the code inside a conditional expression.
    work
  end
end
