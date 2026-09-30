a
 .(args)

Foo
.a
  .b

Foo
.a
  .b(c)

Foo.&(
    foo,
    bar
)

expect { Foo.new }.
  to change { Bar.count }.
      from(1).to(2)
