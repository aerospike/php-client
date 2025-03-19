#!/bin/zsh -m

set +x

SCRIPT_PATH="${0:A:h}"
PROJ_FOLDER="php-client"

echo $SCRIPT_PATH
if [[ $SCRIPT_PATH == *$PROJ_FOLDER* ]]; then
  echo "we are (prolly) SOMEWHERE in the repo!"
  if [[ ${PWD} == *build ]]; then
    echo "script is being run from the build dir!"
    pwd
    cd ..
    echo "now we're in the project root dir!"
    pwd
  else
    echo "script is NOT being run from the build dir!"
fi
else
  echo "we are (definitely) NOT in the repo!"
  pwd
  cd $SCRIPT_PATH/$PROJ_FOLDER
  echo "but NOW we are!"
  pwd
fi
