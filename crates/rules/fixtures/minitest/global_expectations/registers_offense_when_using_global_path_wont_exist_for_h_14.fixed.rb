it 'does something' do
  $n = do_something
  expect($n[:foo]).path_wont_exist 42
end
