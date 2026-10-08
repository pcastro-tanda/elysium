class SomeClass
  EXPIRED_AT ||= 1.week.since
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not assign `since` to constants as it will be evaluated only once.
end
