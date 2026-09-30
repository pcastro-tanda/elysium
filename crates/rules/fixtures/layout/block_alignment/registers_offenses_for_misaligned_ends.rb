def foo(bar)
  bar.get_stuffs
      .reject do |stuff|
        stuff.with_a_very_long_expression_that_doesnt_fit_the_line
        end.select do |stuff|
        ^^^ `end` at 5, 8 is not aligned with `bar.get_stuffs` at 2, 2 or `.reject do |stuff|` at 3, 6.
        stuff.another_very_long_expression_that_doesnt_fit_the_line
    end
    ^^^ `end` at 7, 4 is not aligned with `bar.get_stuffs` at 2, 2 or `end.select do |stuff|` at 5, 8.
      .select do |stuff|
        stuff.another_very_long_expression_that_doesnt_fit_the_line
        end
        ^^^ `end` at 10, 8 is not aligned with `bar.get_stuffs` at 2, 2 or `.select do |stuff|` at 8, 6.
end
