def foo(bar)
  bar.get_stuffs
      .reject do |stuff|
        stuff.with_a_very_long_expression_that_doesnt_fit_the_line
      end.select do |stuff|
        stuff.another_very_long_expression_that_doesnt_fit_the_line
      end
      .select do |stuff|
        stuff.another_very_long_expression_that_doesnt_fit_the_line
      end
end
