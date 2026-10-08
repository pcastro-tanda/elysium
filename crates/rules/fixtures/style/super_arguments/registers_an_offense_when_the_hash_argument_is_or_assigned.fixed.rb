def foo(options, &block)
  options[:key] ||= default

  super
end
