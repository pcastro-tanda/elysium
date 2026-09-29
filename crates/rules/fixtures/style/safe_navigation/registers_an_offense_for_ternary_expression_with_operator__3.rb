FOO::BAR.nil? ? nil : FOO::BAR.*(42)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.
