it 'does something' do
  n = do_something
  value(n).must_be_within_epsilon 42
end
