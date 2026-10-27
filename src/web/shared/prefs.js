function read(key) {
  try {
    return JSON.parse(localStorage.getItem(key));
  } catch {
    return null;
  }
}

function write(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {}
}

export const prefs = {
  get: name => read(`branchsight:${name}`),
  set: (name, value) => write(`branchsight:${name}`, value),
};

export const repoPrefs = path => ({
  get: name => read(`branchsight:${path}:${name}`),
  set: (name, value) => write(`branchsight:${path}:${name}`, value),
});
