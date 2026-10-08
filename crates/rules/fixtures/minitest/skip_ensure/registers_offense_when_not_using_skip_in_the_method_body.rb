def test_skip
  skip 'This test is skipped.'

  begin
    assert 'foo'.present?
  ensure
    do_something
  end
end
