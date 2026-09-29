unless FOO.nil?
^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.
  FOO.bar { |e| e.qux }
end
