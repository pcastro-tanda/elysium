unless FOO::BAR.nil?
^^^^^^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.
  FOO::BAR.bar(baz)
end
