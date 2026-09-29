foo && (foo.bar? || (foo.baz? && foo.quux?))
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.
