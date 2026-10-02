refine Foo do
  prepend Bar
  ^^^^^^^ Use `import_methods` instead of `prepend` because it was removed in Ruby 3.2.
end
