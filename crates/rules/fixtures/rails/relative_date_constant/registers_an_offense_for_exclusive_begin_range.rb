class SomeClass
  TRIAL_PERIOD = 1.day.ago..DateTime.current
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not assign `ago` to constants as it will be evaluated only once.
end
