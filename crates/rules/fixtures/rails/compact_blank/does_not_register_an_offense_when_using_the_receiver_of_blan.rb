def foo(arg)
  collection.reject { |_| arg.blank? }
end
