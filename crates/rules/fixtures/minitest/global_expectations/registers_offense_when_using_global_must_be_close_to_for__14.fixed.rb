it 'does something' do
  $n = do_something
  expect($n[:foo]).must_be_close_to 42
end
