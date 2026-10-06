class SomeClass
  FUTURE_DATE = Time.zone.tomorrow
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not assign `tomorrow` to constants as it will be evaluated only once.
end
