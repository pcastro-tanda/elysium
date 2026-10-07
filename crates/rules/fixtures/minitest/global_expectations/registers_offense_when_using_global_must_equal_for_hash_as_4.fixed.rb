it 'does something' do
  n = do_something
  value(n[:foo]).must_equal 42
end
