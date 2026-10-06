it 'does something' do
  $n = do_something
  value($n).must_be_within_delta 42
end
