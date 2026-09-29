@foo ? @foo.bar : nil
^^^^^^^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.

@foo.nil? ? nil : @foo.bar
^^^^^^^^^^^^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.

!@foo.nil? ? @foo.bar : nil
^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.

!@foo ? nil : @foo.bar
^^^^^^^^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.
