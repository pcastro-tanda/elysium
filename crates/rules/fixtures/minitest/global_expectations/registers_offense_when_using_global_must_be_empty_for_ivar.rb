it 'does something' do
  @n = do_something
  @n.must_be_empty 42
  ^^ Use `_(@n)` instead.
end
