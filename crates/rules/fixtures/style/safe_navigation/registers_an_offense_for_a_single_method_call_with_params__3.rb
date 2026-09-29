if !foo.nil?
^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.
  foo.bar(baz) { |e| e.qux }
end
