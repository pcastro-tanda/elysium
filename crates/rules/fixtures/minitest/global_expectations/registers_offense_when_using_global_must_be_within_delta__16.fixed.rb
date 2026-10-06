it 'does something' do
  $n = do_something
  _($n).must_be_within_delta 42
end
