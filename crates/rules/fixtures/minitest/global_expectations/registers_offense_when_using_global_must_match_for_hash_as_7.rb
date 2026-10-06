it 'does something' do
  @n = do_something
  @n[:foo].must_match 42
  ^^^^^^^^ Use `value(@n[:foo])` instead.
end
