it 'does something' do
  n = do_something
  value(n[:foo]).must_match 42
end
