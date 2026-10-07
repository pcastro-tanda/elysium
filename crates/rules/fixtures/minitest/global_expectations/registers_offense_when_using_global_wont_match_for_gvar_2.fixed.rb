it 'does something' do
  $n = do_something
  expect($n).wont_match 42
end
