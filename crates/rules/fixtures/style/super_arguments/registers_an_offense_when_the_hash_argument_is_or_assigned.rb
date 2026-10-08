def foo(options, &block)
  options[:key] ||= default

  super(options, &block)
  ^^^^^^^^^^^^^^^^^^^^^^ Call `super` without arguments and parentheses when the signature is identical.
end
