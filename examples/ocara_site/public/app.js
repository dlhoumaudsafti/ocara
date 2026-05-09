// =============================================================================
// HTTPServer Demo - JavaScript
// =============================================================================

document.addEventListener('DOMContentLoaded', () => {
    const resultElement = document.getElementById('api-result');
    const btnStatus = document.getElementById('btn-status');
    const btnUsers = document.getElementById('btn-users');
    const btnEcho = document.getElementById('btn-echo');

    // Fonction helper pour afficher le résultat
    function displayResult(data, status = 200) {
        resultElement.textContent = `Status: ${status}\n\n${JSON.stringify(data, null, 2)}`;
    }

    // Fonction helper pour afficher une erreur
    function displayError(error) {
        resultElement.textContent = `Erreur:\n${error.message}`;
    }

    // GET /api/status
    btnStatus.addEventListener('click', async () => {
        try {
            const response = await fetch('/api/status');
            const data = await response.json();
            displayResult(data, response.status);
        } catch (error) {
            displayError(error);
        }
    });

    // GET /api/users
    btnUsers.addEventListener('click', async () => {
        try {
            const response = await fetch('/api/users');
            const data = await response.json();
            displayResult(data, response.status);
        } catch (error) {
            displayError(error);
        }
    });

    // POST /api/echo
    btnEcho.addEventListener('click', async () => {
        try {
            const payload = { message: 'Hello from browser!', timestamp: Date.now() };
            const response = await fetch('/api/echo', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(payload)
            });
            const data = await response.json();
            displayResult(data, response.status);
        } catch (error) {
            displayError(error);
        }
    });

    console.log('🚀 HTTPServer Demo - JavaScript chargé');
    console.log('📁 Fichier servi depuis: /app.js → ./public/app.js');
});
