class SomeClass
  TRIAL_PERIOD = DateTime.current..1.day.since
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not assign `since` to constants as it will be evaluated only once.
end
