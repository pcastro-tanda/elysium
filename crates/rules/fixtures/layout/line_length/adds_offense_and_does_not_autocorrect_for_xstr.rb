str = <<~`HEREDOC`.do_something.with_args(foo: '', bar: '', baz: '')
                                        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Line is too long. [68/40]
  text
HEREDOC
