it 'does something' do
  @n = do_something
  @n[:foo].wont_be_within_delta 42
  ^^^^^^^^ Use `expect(@n[:foo])` instead.
end
