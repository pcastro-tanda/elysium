it 'does something' do
  n = do_something
  value(n[:foo]).wont_equal 42
end
