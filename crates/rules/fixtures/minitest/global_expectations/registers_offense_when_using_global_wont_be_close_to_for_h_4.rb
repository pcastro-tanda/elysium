it 'does something' do
  n = do_something
  n[:foo].wont_be_close_to 42
  ^^^^^^^ Use `value(n[:foo])` instead.
end
