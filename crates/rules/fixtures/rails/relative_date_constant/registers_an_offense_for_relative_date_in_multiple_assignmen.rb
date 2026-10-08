class SomeClass
  START, A, x = 2.weeks.ago, 1.week.since, 5
         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not assign `since` to constants as it will be evaluated only once.
  ^^^^^^^^^^^^^^^^^^^^^^^^^ Do not assign `ago` to constants as it will be evaluated only once.
end
