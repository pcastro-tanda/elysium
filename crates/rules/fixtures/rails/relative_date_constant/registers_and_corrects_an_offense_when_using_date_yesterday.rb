class SomeClass
  RECENT_DATE = Date.yesterday
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not assign `yesterday` to constants as it will be evaluated only once.
end
