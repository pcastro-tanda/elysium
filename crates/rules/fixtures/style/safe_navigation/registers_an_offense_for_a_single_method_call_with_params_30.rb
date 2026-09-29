unless !@foo
^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.
  @foo.bar(baz)
end
