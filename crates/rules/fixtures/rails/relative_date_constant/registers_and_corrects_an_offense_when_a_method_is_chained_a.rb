class SomeClass
  START_DATE = 2.weeks.ago.to_date
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not assign `ago` to constants as it will be evaluated only once.
end
